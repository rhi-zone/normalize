#import <Foundation/Foundation.h>
#import "Shape.h"

@interface Point : NSObject
@property (nonatomic) double x;
@property (nonatomic) double y;
- (instancetype)initWithX:(double)x y:(double)y;
@end

@implementation Point
// Initializes a Point with x and y coordinates.
- (instancetype)initWithX:(double)x y:(double)y {
    self = [super init];
    if (self) {
        _x = x;
        _y = y;
    }
    return self;
}
@end

@interface Circle : NSObject
@property (nonatomic) double radius;
- (instancetype)initWithRadius:(double)radius;
- (double)area;
@end

@implementation Circle
- (instancetype)initWithRadius:(double)radius {
    self = [super init];
    if (self) {
        _radius = radius;
    }
    return self;
}

- (double)area {
    return M_PI * _radius * _radius;
}
@end

double distance(Point *a, Point *b) {
    double dx = b.x - a.x;
    double dy = b.y - a.y;
    return sqrt(dx * dx + dy * dy);
}

NSString *classify(int n) {
    if (n < 0) {
        return @"negative";
    } else if (n == 0) {
        return @"zero";
    } else {
        return @"positive";
    }
}

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        Point *p1 = [[Point alloc] initWithX:3.0 y:4.0];
        Point *p2 = [[Point alloc] initWithX:0.0 y:0.0];
        NSLog(@"distance: %f", distance(p1, p2));
        Circle *c = [[Circle alloc] initWithRadius:5.0];
        NSLog(@"area: %f", [c area]);
        NSLog(@"%@", classify(-3));
    }
    return 0;
}
